#!/usr/bin/env node
/**
 * Minimal mock OIDC provider for Playwright SSO e2e.
 * Endpoints: discovery, authorize (redirect with code), token, userinfo.
 */
import http from 'node:http'
import { URL } from 'node:url'

const PORT = Number(process.env.OIDC_PORT ?? 9090)
const ISSUER = `http://127.0.0.1:${PORT}`

const USER = {
  sub: 'mock-oidc-user-1',
  email: 'sso-user@vulyk.local',
  email_verified: true,
  name: 'SSO Mock User',
  preferred_username: 'sso-user',
}

function b64url(obj) {
  return Buffer.from(JSON.stringify(obj)).toString('base64url')
}

function fakeIdToken() {
  const header = b64url({ alg: 'none', typ: 'JWT' })
  const payload = b64url({
    iss: ISSUER,
    aud: 'vulyk-e2e',
    ...USER,
  })
  return `${header}.${payload}.`
}

function sendJson(res, status, body) {
  const raw = JSON.stringify(body)
  res.writeHead(status, {
    'Content-Type': 'application/json',
    'Cache-Control': 'no-store',
  })
  res.end(raw)
}

const server = http.createServer((req, res) => {
  const url = new URL(req.url ?? '/', ISSUER)

  if (url.pathname === '/.well-known/openid-configuration') {
    return sendJson(res, 200, {
      issuer: ISSUER,
      authorization_endpoint: `${ISSUER}/authorize`,
      token_endpoint: `${ISSUER}/token`,
      userinfo_endpoint: `${ISSUER}/userinfo`,
      jwks_uri: `${ISSUER}/jwks`,
      response_types_supported: ['code'],
      subject_types_supported: ['public'],
      id_token_signing_alg_values_supported: ['none'],
      scopes_supported: ['openid', 'email', 'profile'],
    })
  }

  if (url.pathname === '/authorize') {
    const redirectUri = url.searchParams.get('redirect_uri')
    const state = url.searchParams.get('state') ?? ''
    if (!redirectUri) {
      res.writeHead(400)
      return res.end('missing redirect_uri')
    }
    const target = new URL(redirectUri)
    target.searchParams.set('code', 'mock-auth-code')
    target.searchParams.set('state', state)
    res.writeHead(302, { Location: target.toString() })
    return res.end()
  }

  if (url.pathname === '/token' && req.method === 'POST') {
    let body = ''
    req.on('data', (chunk) => {
      body += chunk
    })
    req.on('end', () => {
      sendJson(res, 200, {
        access_token: 'mock-access-token',
        token_type: 'Bearer',
        expires_in: 3600,
        id_token: fakeIdToken(),
        scope: 'openid email profile',
      })
    })
    return
  }

  if (url.pathname === '/userinfo') {
    return sendJson(res, 200, USER)
  }

  if (url.pathname === '/jwks') {
    return sendJson(res, 200, { keys: [] })
  }

  if (url.pathname === '/health') {
    return sendJson(res, 200, { ok: true })
  }

  res.writeHead(404)
  res.end('not found')
})

server.listen(PORT, '127.0.0.1', () => {
  console.log(`[mock-oidc] listening on ${ISSUER}`)
})
