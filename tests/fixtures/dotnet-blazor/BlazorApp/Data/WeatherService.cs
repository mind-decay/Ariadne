namespace BlazorApp.Data;

public class WeatherForecast
{
    public int TemperatureC { get; set; }
    public string Summary { get; set; }
}

public class WeatherService
{
    public Task<WeatherForecast> GetForecastAsync()
    {
        return Task.FromResult(new WeatherForecast
        {
            TemperatureC = 20,
            Summary = "Mild"
        });
    }
}
