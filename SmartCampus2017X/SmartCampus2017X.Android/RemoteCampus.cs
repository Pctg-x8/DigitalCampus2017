using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;

using Android.App;
using Android.Content;
using Android.OS;
using Android.Runtime;
using Android.Views;
using Android.Widget;
using Android.Webkit;
using System.Threading.Tasks;
using Java.Lang;
using Android.Util;

namespace SmartCampus2017X.Droid.RemoteCampus
{
    /// WebView Controller
    public sealed class Controller
    {
        private WebViewWithEvent eview;
        public Controller(WebViewWithEvent outer) { this.eview = outer; }

        public Controller ClickElement(string selector)
        {
            this.eview.Evaluate($"document.querySelector(\"{selector}\").click()");
            return this;
        }
        public Controller ClickElement(string selector, uint index)
        {
            this.eview.Evaluate($"document.querySelectorAll(\"{selector}\")[{index}].click()");
            return this;
        }
        public async Task<Controller> JumpToAnchorHref(string selector)
        {
            var to = await this.eview.EvaluateAsync<Java.Lang.String>($"document.querySelector(\"{selector}\").getAttribute('href')");
            var to_n = to.Substring(1, to.Length() - 1);
            Log.Debug("app::Controller", $"JumpToAnchorHref => {to_n}");
            this.eview.Navigate(to_n);
            return this;
        }
        public async Task<Controller> JumpToAnchorHref(string selector, uint index)
        {
            var to = await this.eview.EvaluateAsync<Java.Lang.String>($"document.querySelectorAll(\"{selector}\")[{index}].getAttribute('href')");
            var to_n = to.Substring(1, to.Length() - 1);
            Log.Debug("app::Controller", $"JumpToAnchorHref(index) => {to_n}");
            this.eview.Navigate(to_n);
            return this;
        }

        /// <summary>
        /// ページロード完了を待つ
        /// </summary>
        /// <returns>自分</returns>
        public async Task<Controller> WaitPageLoadingAsync()
        {
            await this.eview.PageLoadedUrlAsync(); return this;
        }
    }
    
    public sealed class HomeMenuControl
    {
        const string IntersysLinkPath = "#gnav ul li.menuBlock ul li:first-child a";

        public async Task AccessIntersys(Controller ctrl)
        {
            await ctrl.JumpToAnchorHref(IntersysLinkPath);
            await ctrl.WaitPageLoadingAsync();
        }
    }

    namespace CampusPlan
    {
        public sealed class MainPage
        {
            const string CourseCategoryLinkID     = "#dgSystem__ctl2_lbtnSystemName";
            const string SyllabusCategoryLinkID   = "#dgSystem__ctl3_lbtnSystemName";
            const string AttendanceCategoryLinkID = "#dgSystem__ctl4_lbtnSystemName";

            /// 履修関係セクションへ
            public async Task<CoursePage> AccessCourseCategory(Controller c)
            {
                await c.ClickElement(CourseCategoryLinkID).WaitPageLoadingAsync();
                return new CoursePage();
            }
        }
        public sealed class CoursePage
        {
            // TODO: 履修登録期間中は動かないかもしれないので要確認(確認する術がないけど)
            const string DetailsLinkID = "#dgSystem__ctl2_lbtnPage";

            /// 履修チェック結果の確認ページへ
            public async Task<CourseDetailsPage> AccessDetails(Controller c)
            {
                await c.ClickElement(DetailsLinkID).WaitPageLoadingAsync();
                return new CourseDetailsPage();
            }
        }
        public sealed class CourseDetailsPage
        {
            // ちょっとまってね
        }
    }
}