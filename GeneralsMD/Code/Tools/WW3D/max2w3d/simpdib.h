#ifndef SIMPDIB_H
#define SIMPDIB_H

#ifndef ALWAYS_H
#include "always.h"
#endif

#include <Max.h>

#ifndef WIN_H
#include "win.h"
#endif

#ifndef PALETTE_H
#include "palette.h"
#endif

class SimpleDIBClass
{
public:

	SimpleDIBClass(HWND hwnd,int width, int height,PaletteClass & pal);
	~SimpleDIBClass(void);

	HBITMAP		Get_Handle()		{ return Handle; }
	int			Get_Width(void)		{ return Width; }
	int			Get_Height(void)	{ return Height; }

	void			Clear(unsigned char color);
	void			Set_Pixel(int i,int j,unsigned char color);

private:

	bool					IsZombie;	// object constructor failed, its a living-dead object!
	BITMAPINFO *		Info;			// info used in creating the dib + the palette.
	HBITMAP				Handle;		// handle to the actual dib
	unsigned char *	Pixels;		// address of memory containing the pixel data
	int					Width;		// width of the dib
	int					Height;		// height of the dib
	unsigned char *	PixelBase;	// address of upper left pixel (this and DIBPitch abstract up/down DIBS)
	int					Pitch;		// offset from DIBPixelBase to next row (can be negative for bottom-up DIBS)

};


#endif /*SIMPDIB_H*/