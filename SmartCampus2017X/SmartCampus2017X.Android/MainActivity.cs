﻿using System;

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
using System.Reactive;

namespace SmartCampus2017X.Droid
{
    [Activity(Label = "SmartCampus2017X", Icon = "@drawable/icon", Theme = "@android:style/Theme.Material.Light.DarkActionBar", MainLauncher = true, ConfigurationChanges = ConfigChanges.ScreenSize | ConfigChanges.Orientation)]
    public class MainActivity : global::Xamarin.Forms.Platform.Android.FormsApplicationActivity
    {
        public WebViewWithEvent ScraperMain { get; private set; }
        public WebViewWithEvent ScraperSub  { get; private set; }
        public App appCommon;
        protected override void OnCreate(Bundle bundle)
        {
            /*TabLayoutResource = Resource.Layout.Tabbar;
            ToolbarResource = Resource.Layout.Toolbar;
            */
            base.OnCreate(bundle);

            global::Xamarin.Forms.Forms.Init(this, bundle);
            LoadApplication(this.appCommon = new App());

            this.ScraperMain = new WebViewWithEvent(new WebView(this), "main");
            this.ScraperSub  = new WebViewWithEvent(new WebView(this), "sub");

            /*this.ScraperMain.view.Visibility = ViewStates.Visible;
            this.SetContentView(this.ScraperMain.view);*/

            this.RunSession();
        }

        private const string SharedPrefsName = "com.cterm2.SmartCampus2017X.Users";
        private void RegisterUserKeys((string, string) keys)
        {
            var prefs = this.GetSharedPreferences(SharedPrefsName, FileCreationMode.Private);
            prefs.Edit().PutString("ID", keys.Item1).PutString("Pass", keys.Item2).Commit();
        }
        private void ClearUserKeys()
        {
            var prefs = this.GetSharedPreferences(SharedPrefsName, FileCreationMode.Private);
            prefs.Edit().Remove("ID").Remove("Pass").Commit();
        }
        private (string, string)? LoadUserKeys()
        {
            var prefs = this.GetSharedPreferences(SharedPrefsName, FileCreationMode.Private);
            var (name0, pass0) = (prefs.GetString("ID", null), prefs.GetString("Pass", null));
            if (name0 == null || pass0 == null) return null; else return (name0, pass0);
        }

        private async void RunSession()
        {
            var logoutPoke = new Subject<Unit>();
            (string, string)? loginKeys = this.LoadUserKeys();
            while (true)
            {
                if (!await this.TryAccessHomepage(null))
                {
                    Log.Debug("app", "Trying to autologin...");
                    while (!await this.TryAccessHomepage(loginKeys)) loginKeys = await this.ProcessLoginInput();
                }
                Log.Debug("app", "CampusHomepage loaded?");
                this.RegisterUserKeys(loginKeys.Value);
                var mainController = new RemoteCampus.Controller(this.ScraperMain);
                var subController = new RemoteCampus.Controller(this.ScraperSub);

                RunOnUiThread(() => (this.appCommon.MainPage as MainPage).UpdateAccessingStep(1, 4));
                var homemenu = new RemoteCampus.HomeMenuControl();
                var loActionPath = await homemenu.GetLogoutActionPath(mainController);
                Log.Debug("app", $"Logout from {loActionPath}");
                Action performLogout = async () =>
                {
                    Log.Debug("app", "Logout");
                    this.ScraperMain.Navigate(loActionPath); await mainController.WaitPageLoadingAsync();
                    this.ClearUserKeys(); loginKeys = null;
                    logoutPoke.OnNext(new Unit());
                };
                (this.appCommon.MainPage as MainPage).OnProcessLogout += performLogout;
                var f = await homemenu.AccessIntersys(mainController);
                RunOnUiThread(() => (this.appCommon.MainPage as MainPage).UpdateAccessingStep(2, 4));
                var intersys = await f.ContentControlTo(mainController, this.ScraperMain);
                var fc = await intersys.AccessCourseCategory(mainController);
                RunOnUiThread(() => (this.appCommon.MainPage as MainPage).UpdateAccessingStep(3, 4));
                var course = await fc.ContentControlTo(mainController, this.ScraperMain);
                var cdetails = await course.AccessDetails(mainController);
                RunOnUiThread(() => (this.appCommon.MainPage as MainPage).UpdateAccessingStep(4, 4));
                var courses = await cdetails.ParseCourseTable(mainController);
                this.RunOnUiThread(() => (this.appCommon.MainPage as MainPage).UpdateCells(courses));
                await logoutPoke.FirstAsync();
                (this.appCommon.MainPage as MainPage).OnProcessLogout -= performLogout;
            }
        }
        private async Task<bool> TryAccessHomepage((string, string)? loginKeys)
        {
            this.RunOnUiThread(() =>
            {
                if (loginKeys.HasValue)
                {
                    const string LoginFormID = "loginPage:formId:j_id33", LoginFormPass = "loginPage:formId:j_id34";
                    this.ScraperMain.Evaluate($"document.querySelector('input[name=\"{LoginFormID}\"]').value = '{loginKeys.Value.Item1}'");
                    this.ScraperMain.Evaluate($"with(document.querySelector('input[name=\"{LoginFormPass}\"]')) {{ value = '{loginKeys.Value.Item2}'; focus(); }}");
                    this.ScraperMain.DispatchKeyClick(Keycode.Enter);
                }
                else this.ScraperMain.Navigate("https://dh.force.com/digitalCampus/campusHomepage");
            });

            do
            {
                var currentUrl = await this.ScraperMain.PageLoadedUrlAsync();
                Log.Debug("app", $"CurrentUrl: {currentUrl}");
                if (currentUrl.IndexOf("campuslogin", StringComparison.CurrentCultureIgnoreCase) >= 0) return false;
                if (currentUrl.IndexOf("digitalCampus/campusHomepage", StringComparison.CurrentCultureIgnoreCase) >= 0) return true;
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
            var userinput = new EditText(this.Activity) { Hint = "Student ID" }; userinput.SetSingleLine();
            var passinput = new EditText(this.Activity) { Hint = "Password", InputType = Android.Text.InputTypes.ClassText | Android.Text.InputTypes.TextVariationPassword };
            userinput.ImeOptions = Android.Views.InputMethods.ImeAction.Next;
            passinput.ImeOptions = Android.Views.InputMethods.ImeAction.Go;
            stackview.AddView(userinput); stackview.AddView(passinput); innerView.AddView(stackview);
            return (new AlertDialog.Builder(this.Activity)).SetTitle("Login to DigitalCampus")
                .SetMessage("Required to log in to DigitalCampus").SetView(innerView)
                .SetPositiveButton("Login", (_, __) => (this.PerformedValues as Subject<(string, string)>).OnNext((userinput.Text, passinput.Text)))
                .SetCancelable(false)
                .Create();
        }
    }
    public class WebViewWithEvent
    {
        public readonly WebView view;
        private WebEventReceiver events;
        private WebChromeEventReceiver chEvents;

        public WebViewWithEvent(WebView v, string DebugName)
        {
            this.events = new WebEventReceiver();
            this.view = v;
            this.view.Visibility = ViewStates.Invisible;
            this.view.Settings.JavaScriptEnabled = true;
            this.view.Settings.LoadsImagesAutomatically = false;
            this.view.SetWebViewClient(this.events);
            this.view.SetWebChromeClient(this.chEvents = new WebChromeEventReceiver(DebugName));
        }
        public async Task<string> PageLoadedUrlAsync() => await this.events.LoadingFinished.FirstAsync();
        public async Task WaitPageLoadingCompletedAsync() { await this.chEvents.OnLoadingCompleted.FirstAsync(); }
        public void Navigate(string url) { this.view.LoadUrl(url); }
        public void Evaluate(string js) { this.view.EvaluateJavascript(js, null); }
        public async Task<T> EvaluateAsync<T>(string js) where T: class, IJavaObject
        {
            var c = new TaskCompletionSource<Java.Lang.Object>();
            this.view.EvaluateJavascript(js, new JSValueCallback() { OnReceive = x => c.SetResult(x) });
            return (await c.Task).JavaCast<T>();
        }
        public void DispatchKeyClick(Keycode code)
        {
            this.view.DispatchKeyEvent(new KeyEvent(KeyEventActions.Down, code));
            this.view.DispatchKeyEvent(new KeyEvent(KeyEventActions.Up, code));
        }
    }
    public class WebEventReceiver : WebViewClient
    {
        const long RedirectingDetectionThrottleTime = 100;
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
            }, RedirectingDetectionThrottleTime);
            base.OnPageFinished(view, url);
        }
        public override bool ShouldOverrideUrlLoading(WebView view, IWebResourceRequest request)
        {
            Log.Debug("app", $"WebView OverrideUrlLoading: {request.Url}");
            this.loadingOverrided = request.Url.ToString();
            return false;
        }

        public async Task<string> PageLoadedUrlAsync() => await this.LoadingFinished.FirstAsync();
    }
    public class WebChromeEventReceiver : WebChromeClient
    {
        private readonly string DebugName;
        public WebChromeEventReceiver(string dn) { this.DebugName = dn; }
        public int lastProgress = 1000;
        public IObservable<Unit> OnLoadingCompleted { get; private set; } = new Subject<Unit>();
        public override void OnProgressChanged(WebView view, int newProgress)
        {
            base.OnProgressChanged(view, newProgress);
            // Log.Debug($"ChromeEventReceiver@{DebugName}", $"Progress: {newProgress}");
            if(lastProgress != newProgress)
            {
                lastProgress = newProgress;
                if (newProgress == 100) (this.OnLoadingCompleted as Subject<Unit>).OnNext(new Unit());
            }
        }
    }
    class JSValueCallback : Java.Lang.Object, IValueCallback
    {
        public Action<Java.Lang.Object> OnReceive;
        public void OnReceiveValue(Java.Lang.Object value) { this.OnReceive(value); }
    }
}

