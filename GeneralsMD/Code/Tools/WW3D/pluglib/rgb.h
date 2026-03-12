#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef RGB_H
#define RGB_H

class PaletteClass;
class HSVClass;


/*
**	Each color entry is represented by this class. It holds the values for the color
**	guns. The gun values are recorded in device dependant format, but the interface
**	uses gun values from 0 to 255.
*/
class RGBClass
{
	public:
		RGBClass(void) : Red(0), Green(0), Blue(0) {}
		RGBClass(unsigned char red, unsigned char green, unsigned char blue) : Red(red), Green(green), Blue(blue) {}
		operator HSVClass (void) const;
		RGBClass & operator = (RGBClass const & rgb) {
			if (this == &rgb) return(*this);

			Red = rgb.Red;
			Green = rgb.Green;
			Blue = rgb.Blue;
			return(*this);
		}

		enum {
			MAX_VALUE=255
		};

		void Adjust(int ratio, RGBClass const & rgb);
		int Difference(RGBClass const & rgb) const;
		int Get_Red(void) const {return (Red);}
		int Get_Green(void) const {return(Green);}
		int Get_Blue(void) const {return(Blue);}
		void Set_Red(unsigned char value) {Red = value;}
		void Set_Green(unsigned char value) {Green = value;}
		void Set_Blue(unsigned char value) {Blue = value;}

	private:

		friend class PaletteClass;

		/*
		**	These hold the actual color gun values in machine independant scale. This
		**	means the values range from 0 to 255.
		*/
		unsigned char Red;
		unsigned char Green;
		unsigned char Blue;
};

extern RGBClass const BlackColor;

#endif
