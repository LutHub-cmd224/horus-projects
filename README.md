# HORUS Projects

SaaS laboratoire basé sur la méthode HORUS : **Analyser → Modéliser → Concevoir → Coder → Tester → Déployer**.

## HORUS Stack

- Frontend : React + TypeScript + Vite + Tailwind CSS
- Backend : Rust + Axum
- Base de données : PostgreSQL
- Accès aux données : SQLx (PR-002)
- API : REST
- Conteneurs : Docker Compose
- CI : GitHub Actions

## Démarrage local

```bash
docker compose up --build
```

- Web : http://localhost:3000
- API health : http://localhost:8080/api/v1/health
- PostgreSQL : localhost:5432

## Workflow

Le projet évolue par branches et Pull Requests afin de conserver une traçabilité claire des décisions et changements.

## PR-001 — Bootstrap

Cette PR pose uniquement le socle technique : monorepo, frontend, API, PostgreSQL, Docker et CI. La logique métier arrive dans les PR suivantes.
