"""Fixture exercising Python branch-sensitive extraction."""


class Repository:
    """A data repository."""

    def fetch(self):
        """Fetch a record."""
        query = "select * from users"
        return query

    def _private_method(self):
        return 1

    @deprecated
    def legacy_fetch(self):
        """Deprecated fetcher."""
        return None


def public_function():
    """A free function."""
    return "ok"


def _hidden_function():
    return 0


def test_repository_fetch():
    Repository().fetch()
