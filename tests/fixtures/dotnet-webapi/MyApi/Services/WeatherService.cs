using MyApi.Models;

namespace MyApi.Services;

public class WeatherService : IWeatherService
{
    public WeatherForecast GetForecast()
    {
        return new WeatherForecast
        {
            Date = DateTime.Now,
            TemperatureC = 25,
            Summary = "Sunny"
        };
    }
}
