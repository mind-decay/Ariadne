using MyApp.Services;

namespace MyApp.Tests;

public class AuthTests
{
    [Fact]
    public void Login_ValidCredentials_ReturnsTrue()
    {
        var repo = new Data.UserRepository();
        var service = new AuthService(repo);
        Assert.True(service.Login("admin", "secret"));
    }

    [Fact]
    public void Login_InvalidCredentials_ReturnsFalse()
    {
        var repo = new Data.UserRepository();
        var service = new AuthService(repo);
        Assert.False(service.Login("unknown", "wrong"));
    }
}
