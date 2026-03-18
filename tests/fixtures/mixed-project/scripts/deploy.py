import subprocess
import sys


def deploy(environment: str) -> None:
    """Deploy the application to the specified environment."""
    print(f"Deploying to {environment}...")
    subprocess.run(["echo", f"Deployed to {environment}"], check=True)


if __name__ == "__main__":
    env = sys.argv[1] if len(sys.argv) > 1 else "staging"
    deploy(env)
