/* Pre-paint appearance bootstrap — external so CSP script-src 'self' allows it (audit F6). */
(function () {
  try {
    var KEY = 'vulyk.appearance'
    var raw = localStorage.getItem(KEY)
    var prefs = raw ? JSON.parse(raw) : {}
    var theme = prefs.theme || 'system'
    var fontSize = prefs.fontSize || 100
    var fontFamily = (prefs.fontFamily || 'inter').toLowerCase().replace(/-/g, '_')
    var allowedFonts = { inter: 1, ibm_plex_sans: 1, noto_sans: 1, source_sans_3: 1 }
    if (!allowedFonts[fontFamily]) fontFamily = 'inter'
    var density = prefs.density || 'comfortable'
    var reducedMotion = Object.prototype.hasOwnProperty.call(prefs, 'reducedMotion')
      ? prefs.reducedMotion
      : null
    var dark = window.matchMedia('(prefers-color-scheme: dark)').matches
    var contrast = window.matchMedia('(prefers-contrast: more)').matches
    var motion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    var resolved = theme
    if (theme === 'system') {
      resolved = contrast ? 'high_contrast' : dark ? 'dark' : 'light'
    }
    var tokens = {
      light: {
        bg: '#F7F1E6',
        ink: '#2A2622',
        muted: '#6B6358',
        accent: '#7A5C18',
        elev: '#EFE6D6',
        line: '#E0D6C8',
        on: '#F7F1E6',
        themeColor: '#7A5C18',
        scheme: 'light',
      },
      dark: {
        bg: '#1A1714',
        ink: '#E8E0D4',
        muted: '#A89F90',
        accent: '#D4B56A',
        elev: '#24201B',
        line: '#3A342C',
        on: '#1A1714',
        themeColor: '#1A1714',
        scheme: 'dark',
      },
      high_contrast: {
        bg: '#000000',
        ink: '#FFFFFF',
        muted: '#E0E0E0',
        accent: '#00FFC8',
        elev: '#0A0A0A',
        line: '#FFFFFF',
        on: '#000000',
        themeColor: '#000000',
        scheme: 'dark',
      },
      sepia: {
        bg: '#1A1612',
        ink: '#E8DCC8',
        muted: '#B8A890',
        accent: '#C4893A',
        elev: '#241E18',
        line: '#3D342A',
        on: '#1A1612',
        themeColor: '#1A1612',
        scheme: 'dark',
      },
    }
    var fontStacks = {
      inter: '"Inter", "Segoe UI", system-ui, sans-serif',
      ibm_plex_sans: '"IBM Plex Sans", "Segoe UI", system-ui, sans-serif',
      noto_sans: '"Noto Sans", "Segoe UI", system-ui, sans-serif',
      source_sans_3: '"Source Sans 3", "Segoe UI", system-ui, sans-serif',
    }
    var t = tokens[resolved] || tokens.light
    var root = document.documentElement
    root.setAttribute('data-theme', resolved)
    root.setAttribute('data-theme-choice', theme)
    root.setAttribute('data-density', density)
    root.setAttribute('data-ui-font', fontFamily)
    root.style.setProperty('--font-scale', String(fontSize / 100))
    root.style.setProperty('--font-body', fontStacks[fontFamily] || fontStacks.inter)
    root.style.setProperty('--bg', t.bg)
    root.style.setProperty('--ink', t.ink)
    root.style.setProperty('--muted', t.muted)
    root.style.setProperty('--accent', t.accent)
    root.style.setProperty('--elev', t.elev)
    root.style.setProperty('--line', t.line)
    root.style.setProperty('--accent-on', t.on)
    root.style.colorScheme = t.scheme
    root.style.backgroundColor = t.bg
    root.style.color = t.ink
    if (reducedMotion === true || (reducedMotion == null && motion)) {
      root.setAttribute('data-reduced-motion', 'true')
    }
    var meta = document.querySelector('meta[name="theme-color"]')
    if (meta) meta.setAttribute('content', t.themeColor)
  } catch (e) {
    /* ignore */
  }
})()
