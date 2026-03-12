#ifndef BLIT_H
#define BLIT_H

#include	"blitter.h"
#include	"buff.h"
#include	"trect.h"
#include	"surface.h"

bool Bit_Blit(Surface & dest, Rect const & destrect, Surface const & source, Rect const & sourcerect, Blitter const & blitter);
bool RLE_Blit(Surface & dest, Rect const & destrect, Surface const & source, Rect const & sourcerect, RLEBlitter const & blitter);

//bool Bit_Blit(SurfaceRect & dest, Rect const & ddrect, SurfaceRect const & source, Rect const & ssrect, Blitter const & blitter);
//bool RLE_Blit(SurfaceRect & dest, Rect const & ddrect, SurfaceRect const & source, Rect const & ssrect, RLEBlitter const & blitter);

bool Bit_Blit(Surface & dest, Rect const & dcliprect, Rect const & ddrect, Surface const & source, Rect const & scliprect, Rect const & ssrect, Blitter const & blitter);
bool RLE_Blit(Surface & dest, Rect const & dcliprect, Rect const & ddrect, Surface const & source, Rect const & scliprect, Rect const & ssrect, RLEBlitter const & blitter);


int Buffer_Size(Surface & surface, int width, int height);
bool To_Buffer(Surface const & surface, Rect const & rect, Buffer & buffer);
bool From_Buffer(Surface & surface, Rect const & rect, Buffer const & buffer);



#endif
