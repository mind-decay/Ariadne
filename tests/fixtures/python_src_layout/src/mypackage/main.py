from mypackage.utils import format_name, validate_email


def run():
    name = format_name("John", "Doe")
    valid = validate_email("john@example.com")
    print(f"Name: {name}, Valid: {valid}")


if __name__ == "__main__":
    run()
