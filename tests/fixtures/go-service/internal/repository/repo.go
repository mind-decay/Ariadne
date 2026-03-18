package repository

type User struct {
	ID       int
	Username string
	Email    string
}

type Repository struct {
	users []User
}

func New() *Repository {
	return &Repository{
		users: []User{
			{ID: 1, Username: "admin", Email: "admin@example.com"},
		},
	}
}

func (r *Repository) FindByUsername(username string) *User {
	for _, u := range r.users {
		if u.Username == username {
			return &u
		}
	}
	return nil
}
