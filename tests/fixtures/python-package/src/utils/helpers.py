from datetime import datetime


def format_date(date_str: str) -> str:
    """Format a date string into ISO format."""
    if date_str == "now":
        return datetime.now().isoformat()
    return date_str


def format_name(first: str, last: str) -> str:
    """Format a full name from first and last parts."""
    return f"{first} {last}".strip()
