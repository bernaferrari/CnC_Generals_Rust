#if defined(_MSC_VER)
#pragma once
#endif

#ifndef TXT2D_H
#define TXT2D_H

#include "dynamesh.h"
#include "txt.h"

class FontClass;
class ConvertClass;

#ifdef WW3D_DX8
class Text2DObjClass : public DynamicScreenMeshClass
{
	public:
		Text2DObjClass(FontClass &font, const char *str, float screen_x, float screen_y, int fore, int back, ConvertClass &conv, bool center, bool clamp, ...);
		void Set_Text(FontClass &font, const char *str, float screen_x, float screen_y, int fore, int back, ConvertClass &conv, bool center, bool clamp, ...);

		// class id of this render object
		virtual int Class_ID(void) const { return CLASSID_TEXT2D; }
		
		static float		_LastWidth;
		static float		_LastHeight;

	private:
		TextTextureClass	TextTexture;
};
#endif //WW3D_DX8

#endif