#ifndef HSV_H
#define HSV_H

class RGBClass;
class HSVClass;

/*
**	Each color entry is represented by this class. It holds the values for the color
**	attributes. The values are recorded in a range from 0 to 255 with 255 being the
**	maximum.
*/
class HSVClass
{
	private:
		static HSVClass const BlackColor;

	public:
		HSVClass(void) : Hue(0), Saturation(0), Value(0) {};
		HSVClass(unsigned char hue, unsigned char saturation, unsigned char value) :
				Hue(hue),
				Saturation(saturation),
				Value(value)
			{};
		operator RGBClass (void) const;

		enum {
			MAX_VALUE=255
		};

		void Adjust(int ratio, HSVClass const & hsv);
		int Difference(HSVClass const & hsv) const;
		int Get_Hue(void) const {return(Hue);};
		int Get_Saturation(void) const {return(Saturation);};
		int Get_Value(void) const {return(Value);};
		void Set_Hue(unsigned char value) {Hue = value;}
		void Set_Saturation(unsigned char value) {Saturation = value;}
		void Set_Value(unsigned char value) {Value = value;}

	private:
		unsigned char Hue;
		unsigned char Saturation;
		unsigned char Value;
};

#endif
