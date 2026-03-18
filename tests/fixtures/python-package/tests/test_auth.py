from src.auth.login import login


def test_login_success():
    assert login("admin", "secret") is True


def test_login_failure():
    assert login("user", "wrong") is False
