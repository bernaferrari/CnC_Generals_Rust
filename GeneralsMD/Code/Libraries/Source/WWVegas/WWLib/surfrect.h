#ifndef SURFRECT_H
#define SURFRECT_H

#ifdef NEVER

#include	"surface.h"
#include	"trect.h"
#include	"point.h"
#include	<assert.h>


/*
**	This class encapsulates a surface and a clipping rectangle. It is intended to be used as
**	a convenience for certain operations that need both a surface and a rectangle. Be aware that
**	all surfaces are imbued with a rectangle that is the full size of the surface.
*/
class SurfaceRect {
	public:
		SurfaceRect(Surface * surfaceptr = NULL, Rect * rect = NULL) : SurfacePtr(surfaceptr), Point(0, 0) {
			assert(SurfacePtr != NULL);
			if (rect != NULL) Window = *rect; else Window = SurfacePtr->Get_Rect();
		}
		SurfaceRect(Surface & surface, Rect * rect = NULL) : SurfacePtr(&surface), Point(0, 0) {
			if (rect != NULL) Window = *rect; else Window = SurfacePtr->Get_Rect();
		}
		SurfaceRect(Surface & surface, Rect const & rect) : SurfacePtr(&surface), Window(rect), Point(0, 0) {}
		~SurfaceRect(void) {};

		/*
		**	Pointer to the surface represented by this class.
		*/
		Surface * SurfacePtr;

		/*
		**	Clipping rectangle window into the surface.
		*/
		Rect Window;

		/*
		**	Tracking 2D coordinate into the surface.
		*/
		Point2D Point;

	private:
		/*
		**	Ensures that the copy constructor and assignment operator never
		**	get created for this class.
		*/
		SurfaceRect(SurfaceRect const & rvalue);
		SurfaceRect operator = (SurfaceRect const & rvalue);
};

#endif
#endif
