"""Small shared test helpers (kept out of conftest so importing them doesn't
re-import the conftest module)."""


def auth(secret: str) -> dict:
    return {"Authorization": f"Bearer {secret}"}
