#!/usr/bin/rdmd

module strip_all_paths;

import std.algorithm, std.ascii, std.range;
import std.stdio, std.string, std.path, std.conv;
import file = std.file;
import xml = std.xml;
import zip = std.zlib;

private struct Point
{
	double x = 0, y = 0;

	@property asDescForm(bool withDelimiter = true)() const
	{
		auto s = x.to!string ~ ", " ~ y.to!string;
		return withDelimiter ? s ~ "; " : s;
	}
	// Apply operator each element
	pure opBinary(string op)(in Point other)
	{
		return mixin("Point(this.x" ~ op ~ "other.x, this.y" ~ op ~ "other.y)");
	}
	void opOpAssign(string op)(in Point other)
	{
		this.x = mixin("this.x" ~ op ~ "other.x");
		this.y = mixin("this.y" ~ op ~ "other.y");
	}
}
/// svg data segment parser: take number
double takeNumber(ref string d)
{
	d = d.stripLeft!(x => x.isWhite || x == ',');
	return d.parse!double;
}
/// svg data segment parser: take point(2 numbers)
Point takePoint(ref string d)
{
	immutable p1 = d.takeNumber;
	immutable p2 = d.takeNumber;
	return Point(p1, p2);
}
private auto genBezierContour(Point begin, Point p1, Point p2)
{
	return begin.asDescForm ~ "(" ~ p1.asDescForm ~ p2.asDescForm!false ~ ");\n";
}

void main(string[] args)
{
	if(args.length <= 1)
	{
		writeln("usage>strip_all_paths [input.svg/svgz]");
		return;
	}

	const sz = file.read(args[1]);
	string s;
	try
	{
		auto z = new zip.UnCompress(zip.HeaderFormat.gzip);
		s = cast(string)z.uncompress(sz);
		s ~= cast(string)z.flush();
	}
	catch(const zip.ZlibException _) s = cast(string)sz;

	auto docparse = new xml.DocumentParser(s);
	string[] contours;
	docparse.onEndTag["path"] = (in xml.Element e)
	{
		string d = e.tag.attr["d"];
		string contour = "";
		Point current, begin, smoothVec;
		while(!d.empty)
		{
			// take command
			d = d.stripLeft!(x => x.isWhite || x == ',');
			switch(d.front)
			{
				case 'M': d.popFront(); current  = d.takePoint; begin = current; contour ~= current.asDescForm; break;
				case 'm': d.popFront(); current += d.takePoint; begin = current; contour ~= current.asDescForm; break;
				case 'L': d.popFront(); current  = d.takePoint; contour ~= current.asDescForm; break;
				case 'l': d.popFront(); current += d.takePoint; contour ~= current.asDescForm; break;
				case 'H': d.popFront(); current.x  = d.takeNumber; contour ~= current.asDescForm; break;
				case 'h': d.popFront(); current.x += d.takeNumber; contour ~= current.asDescForm; break;
				case 'V': d.popFront(); current.y  = d.takeNumber; contour ~= current.asDescForm; break;
				case 'v': d.popFront(); current.y += d.takeNumber; contour ~= current.asDescForm; break;
				case 'C':
				{
					d.popFront();
					immutable p1 = d.takePoint; immutable p2 = d.takePoint;
					current = d.takePoint; smoothVec = current - p2;
					contour ~= "(" ~ p1.asDescForm ~ p2.asDescForm!false ~ "); " ~ current.asDescForm;
					break;
				}
				case 'c':
				{
					d.popFront();
					immutable p1 = current + d.takePoint; immutable p2 = current + d.takePoint;
					current += d.takePoint; smoothVec = current - p2;
					contour ~= "(" ~ p1.asDescForm ~ p2.asDescForm!false ~ "); " ~ current.asDescForm;
					break;
				}
				case 'S':
				{
					d.popFront();
					immutable p1 = current + smoothVec; immutable p2 = d.takePoint;
					current = d.takePoint; smoothVec = current - p2;
					contour ~= "(" ~ p1.asDescForm ~ p2.asDescForm!false ~ "); " ~ current.asDescForm;
					break;
				}
				case 's':
				{
					d.popFront();
					immutable p1 = current + smoothVec; immutable p2 = current + d.takePoint;
					current += d.takePoint; smoothVec = current - p2;
					contour ~= "(" ~ p1.asDescForm ~ p2.asDescForm!false ~ "); " ~ current.asDescForm;
					break;
				}
				case 'z': case 'Z':
					d.popFront();
					contour ~= "#";
					contours ~= contour;
					contour = ""; current = begin;
					break;
				default: assert(false, "Unimplemented Segment: " ~ d.front.to!string);
			}
		}
		if(!contour.empty) contours ~= contour;
	};
	docparse.parse();
	write("@invert-y\n", contours.map!(s => "{\n" ~ s.splitLines.map!(x => "  " ~ x).join("\n") ~ "\n}\n").join);
}
