#if defined(_MSC_VER)
#pragma once
#endif

#ifndef DX8_MISSING_TEXTURE_H
#define DX8_MISSING_TEXTURE_H

#include "always.h"

struct IDirect3DTexture8;
struct IDirect3DSurface8;

class MissingTexture
{
public:
	static void _Init();
	static void _Deinit();

	static IDirect3DTexture8* _Get_Missing_Texture();		// Return a reference to missing texture
	static IDirect3DSurface8* _Create_Missing_Surface();	// Create new surface which contain missing texture image
};


#endif // DX8_MISSING_TEXTURE