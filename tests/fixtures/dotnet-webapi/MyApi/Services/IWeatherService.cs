using MyApi.Models;

namespace MyApi.Services;

public interface IWeatherService
{
    WeatherForecast GetForecast();
}
