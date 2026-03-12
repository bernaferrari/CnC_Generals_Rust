#if defined(_MSC_VER)
#pragma once
#endif

#ifndef BMP2D_H
#define BMP2D_H

#include "dynamesh.h"

class Bitmap2DObjClass : public DynamicScreenMeshClass
{
	public:
		Bitmap2DObjClass(const char *filename, float norm_x, float norm_y,
			bool center, bool additive, bool colorizable = false, int width = -1, int height = -1, bool ignore_alpha = false);
		Bitmap2DObjClass(TextureClass *texture, float norm_x, float norm_y,
			bool center, bool additive, bool colorizable = false, bool ignore_alpha = false);
		Bitmap2DObjClass( const Bitmap2DObjClass & src) : DynamicScreenMeshClass(src) {}

		virtual RenderObjClass * 	Clone(void) const;
		virtual int						Class_ID(void) const { return CLASSID_BITMAP2D; }
};

#endif