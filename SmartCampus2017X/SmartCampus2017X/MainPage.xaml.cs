using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using Xamarin.Forms;

namespace SmartCampus2017X
{
    public class TimetableCellData
    {
        private readonly string name, room;
        public string Name => this.name;
        public string RoomInfo => this.room;
        public TimetableCellData(string name, string room) { this.name = name; this.room = room; }
    }
    public class MainPageViewModel : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private void RaisePropertyChanged(string propertyName) { this.PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName)); }

        private bool isRetrievingData = true;
        public bool IsRetrievingData
        {
            get => isRetrievingData;
            set
            {
                this.isRetrievingData = value;
                this.RaisePropertyChanged("IsRetrievingData");
            }
        }
        private string acquiringState = AppResources.st_logging;
        public string AcquiringState
        {
            get => this.acquiringState;
            set
            {
                this.acquiringState = value;
                this.RaisePropertyChanged("AcquiringState");
            }
        }
    }
    public partial class MainPage : ContentPage
    {
        enum Week : int
        {
            Mon = 1, Tue = 2, Wed = 3, Thu = 4, Fri = 5, Sat = 6
        }
        private MainPageViewModel vm;
        public MainPage()
        {
            InitializeComponent();
            this.BindingContext = this.vm = new MainPageViewModel();
            // this.AddCellAt("サンプル講義A 概論", "E15-17", Week.Wed, 2);
        }
        public event Action OnProcessLogout;

        public void UpdateAccessingStep(int current, int max)
        {
            this.vm.AcquiringState = $"{AppResources.st_accessing} {current}/{max}";
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
            this.vm.IsRetrievingData = false;
        }

        public async void DisplayClassInfo(object sender, EventArgs e)
        {
            var c = (sender as StackLayout).Parent.BindingContext as TimetableCellData;
            await DisplayAlert(c.Name, c.RoomInfo, "close");
        }
        public void DoLogout(object sender, EventArgs e)
        {
            this.OnProcessLogout();
            this.vm.IsRetrievingData = true;
            this.vm.AcquiringState = AppResources.st_logging;
        }
        
        private void TapGestureRecognizer_Tapped(object sender, EventArgs e)
        {
            this.DisplayClassInfo(sender, e);
        }
    }
}
