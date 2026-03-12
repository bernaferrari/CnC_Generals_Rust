#ifndef DIB_H
#define DIB_H

#include "always.h"
#include "bsurface.h"
#include "palette.h"
#include "win.h"


class DIB8Class
{
public:

	DIB8Class(HWND hwnd,int width, int height, PaletteClass & pal);
	~DIB8Class(void);

	HBITMAP		Get_Handle()		{ return Handle; }
	int			Get_Width(void)		{ return Width; }
	int			Get_Height(void)	{ return Height; }
	Surface &	Get_Surface(void)	{ return *Surface; }

	void		Clear(unsigned char color);

private:

	bool				IsZombie;	// object constructor failed, its a living-dead object!
	BITMAPINFO *		Info;		// info used in creating the dib + the palette.
	HBITMAP				Handle;		// handle to the actual dib
	unsigned char *		Pixels;		// address of memory containing the pixel data
	int					Width;		// width of the dib
	int					Height;		// height of the dib
	unsigned char *		PixelBase;	// address of upper left pixel (this and DIBPitch abstract up/down DIBS)
	int					Pitch;		// offset from DIBPixelBase to next row (can be negative for bottom-up DIBS)

	BSurface *			Surface;	// Bsurface wrapped around the pixel buffer.
};


#endif /*DIB_H*/