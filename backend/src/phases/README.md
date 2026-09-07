# HORUS Phases Core

The phase engine enforces the ordered HORUS workflow:

1. ANALYZE
2. MODEL
3. DESIGN
4. BUILD
5. TEST
6. DEPLOY

A phase can be started only when AVAILABLE. Required validation criteria must be completed before validation. Validation unlocks the next phase; validating DEPLOY completes the project. OWNER/ADMIN validate phases, MEMBER may progress criteria, VIEWER is read-only.
