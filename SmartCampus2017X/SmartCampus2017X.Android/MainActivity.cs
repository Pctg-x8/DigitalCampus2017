using System;

using Android.App;
using Android.Content.PM;
using Android.Runtime;
using Android.Views;
using Android.Widget;
using Android.OS;
using Android.Webkit;
using Android.Util;
using System.Reactive.Subjects;
using System.Threading.Tasks;
using Android.Content;
using System.Reactive.Linq;
using Android.Graphics.Drawables;
using Android.Graphics;

namespace SmartCampus2017X.Droid
{
    [Activity(Label = "SmartCampus2017X", Icon = "@drawable/icon", Theme = "@android:style/Theme.Material.Light.DarkActionBar", MainLauncher = true, ConfigurationChanges = ConfigChanges.ScreenSize | ConfigChanges.Orientation)]
    public class MainActivity : global::Xamarin.Forms.Platform.Android.FormsApplicationActivity
    {
        private WebView wv;
        protected override async void OnCreate(Bundle bundle)
        {
            /*TabLayoutResource = Resource.Layout.Tabbar;
            ToolbarResource = Resource.Layout.Toolbar;
            */
            base.OnCreate(bundle);

            global::Xamarin.Forms.Forms.Init(this, bundle);
            LoadApplication(new App());

            this.wv = new WebView(this);
            this.wv.Settings.JavaScriptEnabled = true;
            this.wv.Settings.LoadsImagesAutomatically = false;
            this.wv.Visibility = ViewStates.Invisible;
            this.wv.SetWebViewClient(WebEventReceiver.Instance);

            await this.RunSession();
        }

        private async Task RunSession()
        {
            (string, string)? loginKeys = null;
            while (!await this.TryAccessHomepage(loginKeys)) loginKeys = await this.ProcessLoginInput();
            Log.Debug("app", "CampusHomepage loaded?");
        }
        private async Task<bool> TryAccessHomepage((string, string)? loginKeys)
        {
            this.RunOnUiThread(() =>
            {
                if (loginKeys.HasValue)
                {
                    const string LoginFormID = "loginPage:formId:j_id33", LoginFormPass = "loginPage:formId:j_id34";
                    this.wv.EvaluateJavascript($"document.querySelector('input[name=\"{LoginFormID}\"]').value = '{loginKeys.Value.Item1}'", null);
                    this.wv.EvaluateJavascript($"with(document.querySelector('input[name=\"{LoginFormPass}\"]')) {{ value = '{loginKeys.Value.Item2}'; focus(); }}", null);
                    this.wv.DispatchKeyEvent(new KeyEvent(KeyEventActions.Down, Keycode.Enter));
                    this.wv.DispatchKeyEvent(new KeyEvent(KeyEventActions.Up, Keycode.Enter));
                }
                else this.wv.LoadUrl("https://dh.force.com/digitalCampus/campusHomepage");
            });

            do
            {
                var currentUrl = await WebEventReceiver.Instance.LoadingFinished.FirstAsync();
                Log.Debug("app", $"CurrentUrl: {currentUrl}");
                if (currentUrl.Contains("campuslogin")) return false;
                if (currentUrl.Contains("campusHomepage")) return true;
            } while (true);
        }
        private async Task<(string, string)?> ProcessLoginInput()
        {
            var d = new LoginDialogFragment();
            d.Show(this.FragmentManager, "login");
            return await d.PerformedValues.FirstAsync();
        }
    }
    class LoginDialogFragment : DialogFragment
    {
        public IObservable<(string, string)> PerformedValues { get; private set; } = new Subject<(string, string)>();

        public override Dialog OnCreateDialog(Bundle savedInstanceState)
        {
            var dlg = new AlertDialog.Builder(this.Activity);
            var innerView = new FrameLayout(this.Activity);
            var dpm = (int)TypedValue.ApplyDimension(ComplexUnitType.Dip, 24, this.Context.Resources.DisplayMetrics);
            innerView.SetPadding(dpm, 0, dpm, 0);
            var stackview = new LinearLayout(this.Activity) { Orientation = Orientation.Vertical };
            var userinput = new EditText(this.Activity) { Hint = "Student ID" };
            var passinput = new EditText(this.Activity) { Hint = "Password", InputType = Android.Text.InputTypes.ClassText | Android.Text.InputTypes.TextVariationPassword };
            stackview.AddView(userinput); stackview.AddView(passinput); innerView.AddView(stackview);
            return (new AlertDialog.Builder(this.Activity)).SetTitle("Login to DigitalCampus")
                .SetMessage("Required to log in to DigitalCampus").SetView(innerView)
                .SetPositiveButton("Login", (_, __) => (this.PerformedValues as Subject<(string, string)>).OnNext((userinput.Text, passinput.Text)))
                .SetCancelable(false)
                .Create();
        }
    }
    public class WebEventReceiver : WebViewClient
    {
        public static WebEventReceiver Instance { get; private set; } = new WebEventReceiver();

        private string loadingOverrided = null;
        public IObservable<string> LoadingFinished { get; private set; } = new Subject<string>();
        public override void OnPageFinished(WebView view, string url)
        {
            Log.Debug("app", $"WebView Finished: {url}");
            (new Handler()).PostDelayed(() =>
            {
                if(this.loadingOverrided == null || this.loadingOverrided == url)
                {
                    (this.LoadingFinished as Subject<string>).OnNext(url);
                    this.loadingOverrided = null;
                }
            }, 100);
            base.OnPageFinished(view, url);
        }
        public override bool ShouldOverrideUrlLoading(WebView view, IWebResourceRequest request)
        {
            Log.Debug("app", $"WebView OverrideUrlLoading: {request.Url}");
            this.loadingOverrided = request.Url.ToString();
            return false;
        }
    }
}

