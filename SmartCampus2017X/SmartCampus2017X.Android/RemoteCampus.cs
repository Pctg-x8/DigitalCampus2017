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
using Newtonsoft.Json;
using System.Web;

namespace SmartCampus2017X.Droid.RemoteCampus
{
    [JsonObject("TypedContainer")]
    public class TypedContainer<T> { [JsonProperty("value")] public T Value; }

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
        public Task<T> QueryAsync<T>(string q) where T: class, IJavaObject => this.eview.EvaluateAsync<T>(q);
        public async Task<string> QueryAnchorHref(string selector)
        {
            var to = await this.eview.EvaluateAsync<Java.Lang.String>($"({{ value: document.querySelector(\"{selector}\").getAttribute('href') }})");
            return JsonConvert.DeserializeObject<TypedContainer<string>>(to.ToString()).Value;
        }
        public async Task<string> QueryAnchorHref(string selector, uint index)
        {
            var to = await this.eview.EvaluateAsync<Java.Lang.String>($"({{ value: document.querySelectorAll(\"{selector}\")[{index}].getAttribute('href') }})");
            return JsonConvert.DeserializeObject<TypedContainer<string>>(to.ToString()).Value;
        }
        public async Task<Controller> JumpToAnchorHref(string selector)
        {
            this.eview.Navigate(await this.QueryAnchorHref(selector));
            return this;
        }
        public async Task<Controller> JumpToAnchorHref(string selector, uint index)
        {
            this.eview.Navigate(await this.QueryAnchorHref(selector, index));
            return this;
        }

        /// <summary>
        /// ページロード完了を待つ
        /// </summary>
        /// <returns>自分</returns>
        public async Task<Controller> WaitPageLoadingAsync(bool waitOnLoadEvent = false)
        {
            await this.eview.PageLoadedUrlAsync(); if (waitOnLoadEvent) await this.eview.WaitPageLoadingCompletedAsync();
            return this;
        }
    }
    
    public sealed class HomeMenuControl
    {
        const string IntersysLinkPath = "#gnav ul li.menuBlock ul li:first-child a";
        const string LogoutBlockPath = "#gnav ul li.menuBlock.menuBlockLink";

        public async Task<CampusPlan.Frame<CampusPlan.EmptyMenu, CampusPlan.MainPage>> AccessIntersys(Controller ctrl)
        {
            await ctrl.JumpToAnchorHref(IntersysLinkPath);
            await ctrl.WaitPageLoadingAsync();
            return new CampusPlan.Frame<CampusPlan.EmptyMenu, CampusPlan.MainPage>();
        }
        public async Task<string> GetLogoutActionPath(Controller ctrl)
        {
            var v = await ctrl.QueryAsync<Java.Lang.String>($"({{ value: document.querySelectorAll(\"{LogoutBlockPath}\")[1].querySelector('a').href }})");
            return JsonConvert.DeserializeObject<TypedContainer<string>>(v.ToString()).Value;
        }
    }

    namespace CampusPlan
    {
        public sealed class Frame<Menu, Content> where Menu : new() where Content : new()
        {
            public Menu MenuControl => new Menu();
            public Content ContentControl => new Content();

            public async Task<Menu> MenuControlTo(Controller framed, WebViewWithEvent view)
            {
                var src = await framed.QueryAsync<Java.Lang.String>("document.querySelector('frame[name=\"MenuFrame\"]').getAttribute('src')");
                view.Evaluate($"location.href = '{src.Substring(1, src.Length() - 1)}';"); await view.PageLoadedUrlAsync();
                return this.MenuControl;
            }
            public async Task<Content> ContentControlTo(Controller framed, WebViewWithEvent view)
            {
                var src = await framed.QueryAsync<Java.Lang.String>("document.querySelector('frame[name=\"MainFrame\"]').getAttribute('src')");
                var url = src.Substring(1, src.Length() - 1);
                view.Evaluate($"location.href = '{url}';"); await view.PageLoadedUrlAsync();
                return this.ContentControl;
            }
        }
        /// <summary>
        /// ListInput.aspxに対応する(コンテンツが遅延して読み込まれるのでフレームURLの取り方を変えている)
        /// </summary>
        /// <typeparam name="Menu">メニューコンテンツのコントローラ型</typeparam>
        /// <typeparam name="Content">メインコンテンツのコントローラ型</typeparam>
        public sealed class ListInputFrame<Menu, Content> where Menu : new() where Content : new()
        {
            public Menu MenuControl => new Menu();
            public Content ContentControl => new Content();

            public async Task<Menu> MenuControlTo(Controller framed, WebViewWithEvent view)
            {
                var src = await framed.QueryAsync<Java.Lang.String>("MenuFrame.location.href");
                view.Evaluate($"location.href = '{src.Substring(1, src.Length() - 1)}';"); await view.PageLoadedUrlAsync();
                return this.MenuControl;
            }
            public async Task<Content> ContentControlTo(Controller framed, WebViewWithEvent view)
            {
                var src = await framed.QueryAsync<Java.Lang.String>("MainFrame.location.href");
                var url = src.Substring(1, src.Length() - 1);
                view.Evaluate($"location.href = '{url}';"); await view.PageLoadedUrlAsync();
                return this.ContentControl;
            }
        }
        /// <summary>
        /// メニューなし(トップページ)
        /// </summary>
        public sealed class EmptyMenu { }
        // これいる？
        public sealed class Menu
        {
            const string CourseLinkID = "#dtlstMenu__ctl0_lbtnSystemName";
            const string SyllabusLinkID = "#dtlstMenu__ctl1_lbtnSystemName";
            const string AttendanceLinkID = "#dtlstMenu__ctl2_lbynSystemName";

            public Task<string>     GetCourseLinkEntriesLocation(Controller c) => c.QueryAnchorHref(CourseLinkID);
            public Task<string>    GetSllabusLinkEntriesLocation(Controller c) => c.QueryAnchorHref(SyllabusLinkID);
            public Task<string> GetAttendanceLinkEntriesLocation(Controller c) => c.QueryAnchorHref(AttendanceLinkID);
        }
        public sealed class MainPage
        {
            const string CourseCategoryLinkID     = "#dgSystem__ctl2_lbtnSystemName";
            const string SyllabusCategoryLinkID   = "#dgSystem__ctl3_lbtnSystemName";
            const string AttendanceCategoryLinkID = "#dgSystem__ctl4_lbtnSystemName";

            /// 履修関係セクションへ
            public async Task<ListInputFrame<Menu, CoursePage>> AccessCourseCategory(Controller c)
            {
                // onloadで中身が読み込まれるのでそれも待つ
                await c.ClickElement(CourseCategoryLinkID).WaitPageLoadingAsync(true);
                return new ListInputFrame<Menu, CoursePage>();
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
            // ちょっとまってね}

            /// <summary>
            /// 履修テーブルの取得(詳しくはRust側コードを参照)
            /// </summary>
            public async Task<SmartCampus2017X.RemoteCampus.CourseSet> ParseCourseTable(Controller c)
            {
                const string Functions = @"
function null_or_trim(t) { return (!t) ? null : t.textContent.trim(); }
function take_link(k) { return null_or_trim(k.querySelector('a')); }
function take_cell(k) { var link = take_link(k); if(!link) return null; else return { name: link, roominfo: null_or_trim(k.querySelectorAll('.text-kogi-detail')[4]) }; }
";
                const string Komas = @"var tables = document.querySelectorAll('table.rishu-tbl-cell'); tables = [tables[3], tables[5]];
var komas = tables.map(koma => koma.querySelectorAll('td.rishu-tbl-cell'));";
                string Q = $@"{Functions} {Komas}
var first_quarter = [], last_quarter = [];
for(var i = 0; i < komas[0].length; i += 6)
{{
    first_quarter.push({{
        monday:   take_cell(komas[0][i + 0]), tuesday: take_cell(komas[0][i + 1]), wednesday: take_cell(komas[0][i + 2]),
        thursday: take_cell(komas[0][i + 3]), friday:  take_cell(komas[0][i + 4]), saturday:  take_cell(komas[0][i + 5])
    }});
    last_quarter.push({{
        monday:   take_cell(komas[1][i + 0]), tuesday: take_cell(komas[1][i + 1]), wednesday: take_cell(komas[1][i + 2]),
        thursday: take_cell(komas[1][i + 3]), friday:  take_cell(komas[1][i + 4]), saturday:  take_cell(komas[1][i + 5])
    }});
}}
({{ firstQuarter: first_quarter, lastQuarter: last_quarter }})";
                var json_j = await c.QueryAsync<Java.Lang.String>(Q);
                Log.Debug("CourseDetailsPage", $"parsing {json_j.ToString()}");
                return JsonConvert.DeserializeObject<SmartCampus2017X.RemoteCampus.CourseSet>(json_j.ToString());
            }
        }
    }
}