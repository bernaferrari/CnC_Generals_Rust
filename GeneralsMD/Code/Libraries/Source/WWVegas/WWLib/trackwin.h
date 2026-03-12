#ifndef TRACKWIN_H
#define TRACKWIN_H


#ifdef NEVER

#include	"trect.h"

/*
**	This class is used to keep track of a subwindow within a larger window.
*/
class TrackWindow {
	public:
		TrackWindow(int width, int height) : Window(0, 0, width, height), FullWindow(0, 0, width, height) {}

		/*
		**	Simple sub-window manipulation.
		*/
		void Set(Rect const & rect) {Window = rect;if (FullWindow.Width==0) FullWindow = rect;}
		void Reset(void) {Window = Full_Rect();}

		/*
		**	Query the sub-window data.
		*/
		int Get_X(void) const {return(Window.X);}
		int Get_Y(void) const {return(Window.Y);}
		int Get_Width(void) const {return(Window.Width);}
		int Get_Height(void) const {return(Window.Height);}
		Rect Get_Rect(void) const {return(Window);}

		/*
		**	Query the full window data.
		*/
		int Full_Width(void) const {return(FullWindow.Width);}
		int Full_Height(void) const {return(FullWindow.Height);}
		Rect Full_Rect(void) const {return(FullWindow);}

	protected:

		/*
		**	This is the sub-window dimensions.
		*/
		Rect Window;

		/*
		**	This holds the full sized window. It is used for resetting the window
		**	and for maximum window dimension clipping.
		*/
		Rect FullWindow;
};
#endif

#endif
