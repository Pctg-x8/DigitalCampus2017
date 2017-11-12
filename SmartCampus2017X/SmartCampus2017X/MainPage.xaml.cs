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

            this.AddCellAt("サンプル講義A 概論", "E15-17", Week.Wed, 2);
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
    }
}
