#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef BSURFACE_H
#define BSURFACE_H

#include	"buff.h"
#include	"xsurface.h"

/*
**	This class handles a simple surface that exists in system RAM.
*/
class BSurface : public XSurface
{
	public:
		BSurface(int width, int height, int bbp, void * buffer=NULL) : 
			XSurface(width, height), 
			BBP(bbp), 
			Buff(buffer, width * height * bbp)
		{
		}
		
		/*
		**	Gets and frees a direct pointer to the buffer.
		*/
		virtual void * Lock(Point2D point = Point2D(0, 0)) const 
		{
			XSurface::Lock();
			return(((char*)Buff.Get_Buffer()) + point.Y * Stride() + point.X * Bytes_Per_Pixel());
		}

		/*
		**	Queries information about the surface.
		*/
		virtual int Bytes_Per_Pixel(void) const {return(BBP);}
		virtual int Stride(void) const {return(Get_Width() * BBP);}

	protected:

		/*
		**	Recorded bytes per pixel (used when determining pixel positions).
		*/
		int BBP;

		/*
		**	Tracks the buffer that this surface represents.
		*/
		Buffer Buff;
};

#endif
