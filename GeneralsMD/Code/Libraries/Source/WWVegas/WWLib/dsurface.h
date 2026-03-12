#ifndef DSURFACE_H
#define DSURFACE_H

#include	"palette.h"
#include	"win.h"
#include	"xsurface.h"
#include	<ddraw.h>

/*
**	This is a concrete surface class that is based on the DirectDraw
**	API.
*/
class DSurface : public XSurface
{
		typedef XSurface BASECLASS;

	public:
		virtual ~DSurface(void);

		/*
		**	Default constructor.
		*/
		DSurface(void);

		/*
		**	Constructs a working surface (not visible).
		*/
		DSurface(int width, int height, bool system_memory = false, DDPIXELFORMAT *pixform=NULL);

		/*
		**	Creates a surface from a previously created DirectDraw surface object.
		*/
		DSurface(LPDIRECTDRAWSURFACE surfaceptr);

		/*
		** Get/Release a windows device context from a DirectX surface
		*/
		HDC GetDC(void);
		int ReleaseDC(HDC hdc);

		/*
		**	Create a surface object that represents the currently visible screen.
		*/
		static DSurface * Create_Primary(DSurface ** backsurface1=NULL);

		/*
		**	Copies regions from one surface to another.
		*/
		virtual bool Blit_From(Rect const & dcliprect, Rect const & destrect, Surface const & source, Rect const & scliprect, Rect const & sourcerect, bool trans=false);
		virtual bool Blit_From(Rect const & destrect, Surface const & source, Rect const & sourcerect, bool trans=false);
		virtual bool Blit_From(Surface const & source, bool trans=false) {return(XSurface::Blit_From(source, trans));}

		/*
		**	Fills a region with a constant color.
		*/
		virtual bool Fill_Rect(Rect const & rect, int color);
		virtual bool Fill_Rect(Rect const & cliprect, Rect const & fillrect, int color);

		/*
		**	Gets and frees a direct pointer to the video memory.
		*/
		virtual void * Lock(Point2D point = Point2D(0, 0)) const;
		virtual bool Unlock(void) const;

		/*
		**	Queries information about the surface.
		*/
		virtual int Bytes_Per_Pixel(void) const;
		virtual int Stride(void) const;
		bool In_Video_Ram(void) const {return(IsVideoRam);}

		/*
		**	Verifies that this is a direct draw enabled surface.
		*/
		virtual bool Is_Direct_Draw(void) const {return(true);}

		static int Build_Hicolor_Pixel(int red, int green, int blue);
		static void Build_Remap_Table(unsigned short * table, PaletteClass const & palette);
		static unsigned short Get_Halfbright_Mask(void) {return(HalfbrightMask);}
		static unsigned short Get_Quarterbright_Mask(void) {return(QuarterbrightMask);}
		static unsigned short Get_Eighthbright_Mask(void) {return(EighthbrightMask);}

	protected:
		void Restore_Check(void) const;

		/*
		**	Convenient copy of the bytes per pixel value to speed accessing it. It
		**	gets accessed frequently.
		*/
		mutable int BytesPerPixel;

		/*
		**	Lock count and pointer values. This is used to keep track of the levels
		**	of locking the graphic data. This is only here because DirectDraw prohibits
		**	the blitter from working on a surface that has been locked.
		*/
		mutable void * LockPtr;

		/*
		**	If this surface object represents the one that is visible and associated
		**	with the system GDI, then this flag will be true.
		*/
		bool IsPrimary;

		/*
		**	Is this surface represented in video ram?
		*/
		bool IsVideoRam;

		/*
		**	Direct draw specific data.
		*/
		LPDIRECTDRAWSURFACE SurfacePtr;
		DDSURFACEDESC * Description;
		
		/*
		**	Pointer to the clipper object that is attached to the primary
		**	surface.
		*/
		static LPDIRECTDRAWCLIPPER Clipper;

		/*
		**	Pixel format of primary surface.
		*/
		static DDPIXELFORMAT PixelFormat;

		/*
		**	Shift values to extract the gun value from a hicolor pixel such that the 
		**	gun component is normalized to a byte value.
		*/
		static int RedRight;
		static int RedLeft;
		static int BlueRight;
		static int BlueLeft;
		static int GreenRight;
		static int GreenLeft;

	public:
		/*
		** Shift values specific to this surface (the above are for the primary surface)
		*/
		int ThisRedRight;
		int ThisRedLeft;
		int ThisBlueRight;
		int ThisBlueLeft;
		int ThisGreenRight;
		int ThisGreenLeft;

	protected:
		static unsigned short HalfbrightMask;
		static unsigned short QuarterbrightMask;
		static unsigned short EighthbrightMask;


		/*
		** Number of locks we had to remove in order to get the device context...
		*/
		int	DCUnlockCount;

	private:
		/*
		**	This prevents the creation of a surface in ways that are not
		**	supported.
		*/
		DSurface(DSurface const & rvalue);
		DSurface const operator = (DSurface const & rvalue);
};

#endif
