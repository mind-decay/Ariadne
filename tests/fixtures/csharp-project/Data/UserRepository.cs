namespace MyApp.Data;

public class User
{
    public int Id { get; set; }
    public string Username { get; set; } = "";
    public string Email { get; set; } = "";
}

public class UserRepository
{
    private readonly List<User> _users = new()
    {
        new User { Id = 1, Username = "admin", Email = "admin@example.com" }
    };

    public User? FindByUsername(string username)
    {
        return _users.FirstOrDefault(u => u.Username == username);
    }
}
