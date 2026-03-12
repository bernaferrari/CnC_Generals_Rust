#ifndef TRACKXY_H
#define TRACKXY_H

#ifdef NEVER
/*
**	This class is used to keep track of a "current XY location". The Surface class uses this, but
**	it can be used for any such purpose.
*/
class TrackXY {
	public:
		TrackXY(void) : X(0), Y(0) {}

		/*
		**	It is often convenient to have a "current location" for a surface. The
		**	use of this location is arbitrary and outside the scope of this class.
		*/
		void Set(int x, int y) {X = x;Y = y;}
		int Get_X(void) const {return(X);}
		int Get_Y(void) const {return(Y);}

	protected:

		/*
		**	Keeps track of the current location on this surface. The use of this
		**	current location is outside the scope of this class, but it can be quite
		**	useful for other support functions.
		*/
		int X;
		int Y;
};
#endif

#endif
