//! Automatic case key allocation: PREFIX-N with per-project monotonic counter.

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "of", "and", "or", "for", "to", "in", "on", "at", "by", "with", "from",
];

/// Derive a case-key prefix from a project display name.
///
/// Rules:
/// 1. Split on whitespace.
/// 2. Strip surrounding punctuation from each token; hyphenated compounds count as one word.
/// 3. Skip empty tokens, pure digit tokens (years), and English stop words.
/// 4. Take the first alphabetic character of each remaining token (Unicode letter → uppercase).
/// 5. If fewer than 2 characters, take further letters from the first significant word until
///    length ≥ 2 (fallback `"PR"`).
/// 6. Cap at 8 characters.
///
/// Examples: `"Kyiv Open-Air Concert 2026"` → `"KOC"`.
pub fn derive_prefix_from_name(name: &str) -> String {
    let mut initials: Vec<char> = Vec::new();
    let mut significant: Vec<String> = Vec::new();

    for raw in name.split_whitespace() {
        let token: String = raw
            .chars()
            .skip_while(|c| !c.is_alphanumeric())
            .collect::<String>()
            .chars()
            .rev()
            .skip_while(|c| !c.is_alphanumeric())
            .collect::<String>()
            .chars()
            .rev()
            .collect();

        if token.is_empty() {
            continue;
        }
        if token.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let lower = token.to_lowercase();
        if STOP_WORDS.contains(&lower.as_str()) {
            continue;
        }

        let letters: String = token.chars().filter(|c| c.is_alphabetic()).collect();
        if letters.is_empty() {
            continue;
        }
        significant.push(letters.clone());
        if let Some(ch) = letters.chars().next() {
            if let Some(upper) = ch.to_uppercase().next() {
                initials.push(upper);
            }
        }
    }

    let mut prefix: String = initials.into_iter().collect();
    if prefix.chars().count() < 2 {
        if let Some(first) = significant.first() {
            prefix = first
                .chars()
                .flat_map(|c| c.to_uppercase())
                .take(3)
                .collect();
        }
    }
    if prefix.chars().count() < 2 {
        prefix = "PR".into();
    }
    prefix = prefix.chars().take(8).collect();
    // Ensure first char is a letter and rest are A-Z0-9 (digits unlikely from letters).
    let cleaned: String = prefix
        .chars()
        .enumerate()
        .filter_map(|(i, c)| {
            let u = c.to_ascii_uppercase();
            if i == 0 {
                if u.is_ascii_alphabetic() {
                    Some(u)
                } else {
                    None
                }
            } else if u.is_ascii_alphanumeric() {
                Some(u)
            } else {
                None
            }
        })
        .collect();
    if cleaned.len() < 2 {
        "PR".into()
    } else {
        cleaned
    }
}

/// Allocate next case key for a project inside an open transaction.
/// Uses `UPDATE … RETURNING` so concurrent creates never collide.
pub async fn allocate_case_key(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
) -> AppResult<(String, i32)> {
    let row: Option<(String, i32)> = sqlx::query_as(
        r#"UPDATE projects
           SET next_case_number = next_case_number + 1, updated_at = NOW()
           WHERE id = $1
           RETURNING case_key_prefix, next_case_number"#,
    )
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?;

    let (prefix, number) = row.ok_or_else(|| AppError::NotFound("project".into()))?;
    let key = format!("{prefix}-{number}");
    Ok((key, number))
}

/// Insert a tombstone for a deleted case key so it can never be reissued.
pub async fn retire_case_key(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    key: &str,
) -> AppResult<()> {
    let number = parse_case_number(key).unwrap_or(0);
    sqlx::query(
        r#"INSERT INTO case_key_tombstones (project_id, key, case_number)
           VALUES ($1, $2, $3)
           ON CONFLICT (project_id, key) DO NOTHING"#,
    )
    .bind(project_id)
    .bind(key)
    .bind(number)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub fn parse_case_number(key: &str) -> Option<i32> {
    let (_, num) = key.rsplit_once('-')?;
    num.parse().ok()
}

/// Pick a unique prefix for a new project (derive + dedupe against DB).
pub async fn unique_prefix_for_name(pool: &PgPool, name: &str) -> AppResult<String> {
    unique_prefix(pool, &derive_prefix_from_name(name), None).await
}

pub async fn unique_prefix(
    pool: &PgPool,
    candidate: &str,
    exclude_project: Option<Uuid>,
) -> AppResult<String> {
    let cleaned: String = candidate
        .chars()
        .map(|c| c.to_ascii_uppercase())
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect();
    let base = if cleaned.len() < 2 || !cleaned.chars().next().unwrap().is_ascii_alphabetic() {
        "PR".to_string()
    } else {
        cleaned
    };

    let stem: String = base.chars().take(6).collect();
    let mut attempt = base.clone();
    let mut n = 2u32;
    loop {
        let exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(
                 SELECT 1 FROM projects
                 WHERE case_key_prefix = $1
                   AND ($2::uuid IS NULL OR id <> $2)
               )"#,
        )
        .bind(&attempt)
        .bind(exclude_project)
        .fetch_one(pool)
        .await?;
        if !exists {
            return Ok(attempt);
        }
        let suffix = n.to_string();
        let keep = 8usize.saturating_sub(suffix.len()).max(1);
        let stem_trim: String = stem.chars().take(keep).collect();
        attempt = format!("{stem_trim}{suffix}");
        n += 1;
        if n > 10_000 {
            return Err(AppError::Conflict(
                "unable to allocate unique case key prefix".into(),
            ));
        }
    }
}

/// Validate an admin-supplied prefix edit.
pub fn validate_prefix(prefix: &str) -> AppResult<String> {
    let cleaned: String = prefix.trim().to_uppercase();
    if !regex_is_valid_prefix(&cleaned) {
        return Err(AppError::BadRequest(
            "case_key_prefix must match ^[A-Z][A-Z0-9]{1,7}$".into(),
        ));
    }
    Ok(cleaned)
}

/// Validate a project key (`^[A-Z][A-Z0-9_]{1,31}$`).
pub fn validate_project_key(key: &str) -> AppResult<String> {
    let cleaned: String = key.trim().to_uppercase();
    if !regex_is_valid_project_key(&cleaned) {
        return Err(AppError::BadRequest(
            "project key must match ^[A-Z][A-Z0-9_]{1,31}$".into(),
        ));
    }
    Ok(cleaned)
}

fn regex_is_valid_project_key(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    let rest: Vec<char> = chars.collect();
    if rest.is_empty() || rest.len() > 31 {
        return false;
    }
    rest.into_iter()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Derive a unique project `key` from a display name (same initials rules as case_key_prefix).
pub async fn unique_key_for_name(pool: &PgPool, name: &str) -> AppResult<String> {
    unique_project_key(pool, &derive_prefix_from_name(name)).await
}

pub async fn unique_project_key(pool: &PgPool, candidate: &str) -> AppResult<String> {
    let cleaned: String = candidate
        .chars()
        .map(|c| c.to_ascii_uppercase())
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(32)
        .collect();
    let base = if cleaned.len() < 2 || !cleaned.chars().next().unwrap().is_ascii_alphabetic() {
        "PR".to_string()
    } else {
        cleaned
    };

    let stem: String = base.chars().take(28).collect();
    let mut attempt = base.clone();
    let mut n = 2u32;
    loop {
        let exists: bool =
            sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM projects WHERE key = $1)"#)
                .bind(&attempt)
                .fetch_one(pool)
                .await?;
        if !exists {
            return Ok(attempt);
        }
        let suffix = n.to_string();
        let keep = 32usize.saturating_sub(suffix.len()).max(1);
        let stem_trim: String = stem.chars().take(keep).collect();
        attempt = format!("{stem_trim}{suffix}");
        n += 1;
        if n > 10_000 {
            return Err(AppError::Conflict(
                "unable to allocate unique project key".into(),
            ));
        }
    }
}

fn regex_is_valid_prefix(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    let rest: Vec<char> = chars.collect();
    if rest.is_empty() || rest.len() > 7 {
        return false;
    }
    rest.into_iter()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kyiv_concert_prefix() {
        assert_eq!(derive_prefix_from_name("Kyiv Open-Air Concert 2026"), "KOC");
    }

    #[test]
    fn skips_stop_words_and_years() {
        assert_eq!(
            derive_prefix_from_name("The Annual Test of Systems 2025"),
            "ATS"
        );
    }

    #[test]
    fn short_name_pads_from_word() {
        assert_eq!(derive_prefix_from_name("Alpha"), "ALP");
    }

    #[test]
    fn empty_falls_back() {
        assert_eq!(derive_prefix_from_name("123 !!!"), "PR");
    }

    #[test]
    fn validate_prefix_ok() {
        assert_eq!(validate_prefix("koc").unwrap(), "KOC");
    }

    #[test]
    fn validate_prefix_rejects() {
        assert!(validate_prefix("x").is_err());
        assert!(validate_prefix("1AB").is_err());
    }

    #[test]
    fn validate_project_key_ok() {
        assert_eq!(
            validate_project_key("kyiv_concert").unwrap(),
            "KYIV_CONCERT"
        );
        assert_eq!(validate_project_key("DEMO").unwrap(), "DEMO");
    }

    #[test]
    fn validate_project_key_rejects() {
        assert!(validate_project_key("x").is_err());
        assert!(validate_project_key("1AB").is_err());
        assert!(validate_project_key("has-dash").is_err());
    }
}
