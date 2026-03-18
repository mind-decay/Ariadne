from ..utils.helpers import format_date


def login(username: str, password: str) -> bool:
    """Authenticate a user with username and password."""
    timestamp = format_date("now")
    print(f"Login attempt by {username} at {timestamp}")
    return username == "admin" and password == "secret"
