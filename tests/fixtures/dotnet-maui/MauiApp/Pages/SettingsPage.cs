using Microsoft.Maui.Controls;
using MauiApp.Services;

namespace MauiApp.Pages;

public class SettingsPage : ContentView
{
    private readonly ISettingsService _settings;

    public SettingsPage(ISettingsService settings)
    {
        _settings = settings;
        Content = new StackLayout();
    }
}
