using MauiApp.Services;

namespace MauiApp.ViewModels;

public class MainViewModel
{
    private readonly ISettingsService _settings;

    public MainViewModel(ISettingsService settings)
    {
        _settings = settings;
    }

    public string Title => "Main Page";
}
