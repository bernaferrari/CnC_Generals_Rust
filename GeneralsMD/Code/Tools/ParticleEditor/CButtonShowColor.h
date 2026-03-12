#pragma once

#ifndef _H_CBUTTONSHOWCOLOR_
#define _H_CBUTTONSHOWCOLOR_

#include "Lib/Basetype.h"

class CButtonShowColor : public CButton
{
	protected:
		RGBColor m_color;

	public:
		const RGBColor& getColor(void) const { return m_color; }
		void setColor(Int color) { m_color.setFromInt(color); }
		void setColor(const RGBColor& color) { m_color = color; }
		~CButtonShowColor();
		
		
		static COLORREF RGBtoBGR(Int color);
		static Int BGRtoRGB(COLORREF color);


	protected:
		afx_msg void OnPaint();

	DECLARE_MESSAGE_MAP();
};

#endif /* _H_CBUTTONSHOWCOLOR_ */