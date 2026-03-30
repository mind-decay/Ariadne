package auth

func Authenticate(user, pass string) bool {
	return user == "admin" && pass == "secret"
}
