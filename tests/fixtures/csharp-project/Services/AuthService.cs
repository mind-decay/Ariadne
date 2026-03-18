namespace MyApp.Services;

using MyApp.Data;

public class AuthService
{
    private readonly UserRepository _repo;

    public AuthService(UserRepository repo)
    {
        _repo = repo;
    }

    public bool Login(string username, string password)
    {
        var user = _repo.FindByUsername(username);
        return user != null;
    }
}
