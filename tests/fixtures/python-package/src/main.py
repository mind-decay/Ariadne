from .auth import login
from .utils.helpers import format_date


def main():
    result = login.login("admin", "secret")
    today = format_date("2026-03-18")
    print(f"Login: {result}, Date: {today}")


if __name__ == "__main__":
    main()
