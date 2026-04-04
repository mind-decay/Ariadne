using Microsoft.Maui.Controls;
using MauiApp.ViewModels;

namespace MauiApp.Pages;

public class MainPage : ContentPage
{
    public MainPage(MainViewModel viewModel)
    {
        BindingContext = viewModel;
        Content = new StackLayout
        {
            Children = { new Label { Text = "Welcome to MAUI" } }
        };
    }
}
