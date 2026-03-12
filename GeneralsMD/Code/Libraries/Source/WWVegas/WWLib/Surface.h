#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef SURFACE_H
#define SURFACE_H

#include	"point.h"
#include	"trect.h"

/*
**	This is an abstract interface class for a graphic surface. Graphic operations will use this
**	interface to perform their function. The philosphy behind this interface is that it represents
**	a small but useful set of functions. Emphasis is placed on supporting those functions which are
**	likely to have hardware assist.
*/
class Surface
{
	public:
		Surface(int width, int height) : Width(width), Height(height) {}
		virtual ~Surface(void) {};
		
		/*
		**	Copies regions from one surface to another.
		*/
		virtual bool Blit_From(Rect const & dcliprect, Rect const & destrect, Surface const & source, Rect const & scliprect, Rect const & sourcerect, bool trans=false) = 0;
		virtual bool Blit_From(Rect const & destrect, Surface const & source, Rect const & sourcerect, bool trans=false) = 0;
		virtual bool Blit_From(Surface const & source, bool trans=false) = 0;

		/*
		**	Fills a region with a constant color.
		*/
		virtual bool Fill_Rect(Rect const & rect, int color) = 0;
		virtual bool Fill_Rect(Rect const & cliprect, Rect const & fillrect, int color) = 0;
		virtual bool Fill(int color) = 0;

		/*
		**	Fetches and stores a pixel to the display (pixel is in surface format).
		*/
		virtual bool Put_Pixel(Point2D const & point, int color) = 0;
		virtual int Get_Pixel(Point2D const & point) const = 0;

		/*
		**	Draws lines onto the surface.
		*/
		virtual bool Draw_Line(Point2D const & startpoint, Point2D const & endpoint, int color) = 0;
		virtual bool Draw_Line(Rect const & cliprect, Point2D const & startpoint, Point2D const & endpoint, int color) = 0;
		
		/*
		**	Draws rectangle onto the surface.
		*/
		virtual bool Draw_Rect(Rect const & rect, int color) = 0;
		virtual bool Draw_Rect(Rect const & cliprect, Rect const & rect, int color) = 0;

		/*
		**	Gets and frees a direct pointer to the video memory.
		*/
		virtual void * Lock(Point2D point = Point2D(0, 0)) const = 0;
		virtual bool Unlock(void) const = 0;
		virtual bool Is_Locked(void) const = 0;

		/*
		**	Queries information about the surface.
		*/
		virtual int Bytes_Per_Pixel(void) const = 0;
		virtual int Stride(void) const = 0;
		virtual Rect Get_Rect(void) const {return(Rect(0, 0, Width, Height));}
		virtual int Get_Width(void) const {return(Width);}
		virtual int Get_Height(void) const {return(Height);}
		
		/*
		**	Hack function to serve the purpose that RTTI was invented for, but since
		**	the Watcom compiler doesn't support RTTI, we must resort to using this
		**	alternative.
		*/
		virtual bool Is_Direct_Draw(void) const {return(false);}

	protected:

		/*
		**	Records logical pixel dimensions of the surface.
		*/
		int Width;
		int Height;
};


#endif
