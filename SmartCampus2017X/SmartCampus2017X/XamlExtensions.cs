using System;
using Xamarin.Forms;
using Xamarin.Forms.Xaml;

namespace SmartCampus2017X.Extensions.Xaml
{
    [ContentProperty("Key")]
    public sealed class Translate : IMarkupExtension
    {
        public string Key { get; set; }

        public object ProvideValue(IServiceProvider serviceProvider)
        {
            if (string.IsNullOrEmpty(this.Key)) throw new ArgumentNullException("Key");
            
            return AppResources.ResourceManager.GetString(this.Key) ?? $"??{this.Key}??";
        }
    }
}