# Deploying Orbital Sandbox

This repo can be shipped as a single Docker image that serves **both** the Rust
backend (Actix-web) and the compiled React frontend.

---

## 1. Build the image locally

```bash
# from repo root
docker build -t orbital-sandbox .
```

## 2. Run it

```bash
docker run -p 8080:8080 orbital-sandbox  # visit http://localhost:8080
```

The container uses the `PORT` environment variable (default 8080).

```bash
docker run -e PORT=9000 -p 9000:9000 orbital-sandbox
```

---

## 3. Deploy to Render (free web service)

1.  Push this repository to GitHub.
2.  Create a new **Web Service** on https://dashboard.render.com
    • Environment: `Docker`  
    • Build Command: _(leave blank – Render builds automatically)_  
    • Start Command: _(leave blank because `CMD` in Dockerfile starts server)_
3.  Save and let Render build + deploy.
4.  Your shareable link will look like `https://orbital-sandbox.onrender.com`.

> **Other hosts** (Railway, Fly.io, Google Cloud Run, AWS App Runner) accept the
> same Dockerfile – just point them to the repo and they will build.

---

## 4. Deploy frontend only (optional)

If you prefer serverless hosting for the UI (e.g. Netlify) keep the backend on
Render and set the API URL in `vite.config.js` to point to the Render domain.

```js
proxy: { '/api': 'https://orbital-sandbox.onrender.com' }
```

Then

```bash
cd web
npm run build
netlify deploy --dir=dist
```

---
