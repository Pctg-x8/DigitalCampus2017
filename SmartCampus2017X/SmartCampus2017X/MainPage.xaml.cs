using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using Xamarin.Forms;

namespace SmartCampus2017X
{
    public class TimetableCellData
    {
        private string name, room;
        public string Name { get => this.name; }
        public string RoomInfo { get => this.room; }
        public TimetableCellData(string name, string room) { this.name = name; this.room = room; }
    }
    public partial class MainPage : ContentPage
    {
        enum Week : int
        {
            Mon = 1, Tue = 2, Wed = 3, Thu = 4, Fri = 5, Sat = 6
        }
        public MainPage()
        {
            InitializeComponent();

            // this.AddCellAt("サンプル講義A 概論", "E15-17", Week.Wed, 2);
        }

        private void AddCellAt(string name, string roomInfo, Week week, int row)
        {
            var cv = new ContentView()
            {
                ControlTemplate = this.Resources["TimetableCell"] as ControlTemplate,
                BindingContext = new TimetableCellData(name, roomInfo)
            };
            Grid.SetColumn(cv, (int)week); Grid.SetRow(cv, row);
            this.Timetable.Children.Add(cv);
        }
        public void UpdateCells(RemoteCampus.CourseSet set)
        {
            foreach (var (clist, row) in set.FirstQuarter.Select((a, b) => (a, b + 1)))
            {
                foreach(var (c, col) in clist.Enumerate().Select((a, b) => (a, b + 1)).Where(a => a.Item1 != null))
                {
                    // foreach (var old in this.Timetable.Children.Where(x => Grid.GetColumn(x) == col && Grid.GetRow(x) == row).ToArray()) this.Timetable.Children.Remove(old);
                    if (row > 1 && set.FirstQuarter[row - 2][col] != null && set.FirstQuarter[row - 2][col].Name == c.Name)
                    {
                        // extend
                        Grid.SetRowSpan(this.Timetable.Children.Where(x => Grid.GetColumn(x) == col && Grid.GetRow(x) == row - 1).First(), 2);
                    }
                    else this.AddCellAt(c.Name, c.RoomInfo, (Week)col, row);
                }
            }
        }

        public async void DisplayClassInfo(object sender, EventArgs e)
        {
            var c = (sender as StackLayout).Parent.BindingContext as TimetableCellData;
            await DisplayAlert(c.Name, c.RoomInfo, "close");
        }
        
        private void TapGestureRecognizer_Tapped(object sender, EventArgs e)
        {
            this.DisplayClassInfo(sender, e);
        }
    }
}
