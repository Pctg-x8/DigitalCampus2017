using Newtonsoft.Json;
using System;
using System.Collections.Generic;

namespace SmartCampus2017X.RemoteCampus
{
    [JsonObject("course")]
    public class Course
    {
        [JsonProperty("name")] public string Name { get; set; }
        [JsonProperty("roominfo")] public string RoomInfo { get; set; }
    }
    [JsonObject("weeklyCourses")]
    public class WeeklyCourses
    {
        [JsonProperty("monday")] public Course Monday { get; set; }
        [JsonProperty("tuesday")] public Course Tuesday { get; set; }
        [JsonProperty("wednesday")] public Course Wednesday { get; set; }
        [JsonProperty("thursday")] public Course Thursday { get; set; }
        [JsonProperty("friday")] public Course Friday { get; set; }
        [JsonProperty("saturday")] public Course Saturday { get; set; }

        public IEnumerable<Course> Enumerate()
        {
            yield return this.Monday;
            yield return this.Tuesday;
            yield return this.Wednesday;
            yield return this.Thursday;
            yield return this.Friday;
            yield return this.Saturday;
        }
        public Course this[int index]
        {
            get
            {
                switch(index)
                {
                    case 1: return this.Monday;
                    case 2: return this.Tuesday;
                    case 3: return this.Wednesday;
                    case 4: return this.Thursday;
                    case 5: return this.Friday;
                    case 6: return this.Saturday;
                    default: throw new IndexOutOfRangeException("index must be in 1-6(inclusive)");
                }
            }
        }
    }
    [JsonObject("courseset")]
    public class CourseSet
    {
        [JsonProperty("firstQuarter")] public List<WeeklyCourses> FirstQuarter { get; set; }
        [JsonProperty("lastQuarter")] public List<WeeklyCourses> LastQuarter { get; set; }
    }
}