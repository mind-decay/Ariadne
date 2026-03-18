using MyApp.Services;
using MyApp.Data;

namespace MyApp;

class Program
{
    static void Main(string[] args)
    {
        var repo = new UserRepository();
        var auth = new AuthService(repo);
        var result = auth.Login("admin", "secret");
        Console.WriteLine($"Login result: {result}");
    }
}
