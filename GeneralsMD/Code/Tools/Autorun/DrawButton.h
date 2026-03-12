#ifndef  DRAWBUTTONS_H
#define  DRAWBUTTONS_H

#include <tchar.h>
#include <stdlib.h>
#include "ttfont.h"


//-------------------------------------------------------------------------
// Custom "Button" Class
//-------------------------------------------------------------------------
class DrawButton
{
	public:
   
		enum BUTTON_STATE {
   			NORMAL_STATE = 0,
	      	PRESSED_STATE,
   			FOCUS_STATE
		};

		DrawButton ( int id, RECT button_rect,  char *normal, char *focus, char *pressed, const char *string, TTFontClass *fontptr );
		DrawButton ( int id, RECT button_rect,  char *normal, char *focus, char *pressed, const wchar_t *string, TTFontClass *fontptr );

		char			*Return_Normal_Bitmap	( void )						{ return NormalBitmap;	};
		char			*Return_Pressed_Bitmap	( void )						{ return PressedBitmap; };
		char			*Return_Focus_Bitmap 	( void )						{ return FocusBitmap;	};
		char			*Return_Bitmap 			( void );

		bool	 		Draw_Bitmaps		   	( void )						{ return( UseBitmaps ); };

		void			Draw_Text				( HDC hDC );

		BUTTON_STATE 	Get_State			   	( void )						{ return ButtonState;	};
		bool	 		Is_Mouse_In_Region 		( int mouse_x, int mouse_y );
		int				Return_Id			   	( void )						{ return Id; };
		int				Return_X_Pos 		   	( void )						{ return rect.left; };
		int				Return_Y_Pos		   	( void )						{ return rect.top; };
		int				Return_Width		   	( void )						{ return( rect.right - rect.left ); };
		int				Return_Height		   	( void )						{ return( rect.bottom - rect.top ); };
		int				Return_Stretched_Width	( void )						{ return( StretchedWidth ); };
		int				Return_Stretched_Height	( void )						{ return( StretchedHeight ); };
		void	 		Return_Area			   	( RECT *area );
		void	 		Return_Area			   	( Rect *area );
		void	 		Return_Text_Area	   	( Rect *area );
		TTFontClass		*Return_Font_Ptr		( void )						{ return( MyFontPtr ); };
		wchar_t			*Return_Text			( void )						{ return( String ); };
		void	 		Set_State				( BUTTON_STATE state )			{ ButtonState = state; };
		int				Set_Stretched_Width		( int  );      
		int				Set_Stretched_Height	( int  );      

	protected:

   		int				Id;
		Rect			MyRect;
		Rect			TextRect;
		RECT			rect;
		BUTTON_STATE	ButtonState;
		int				StretchedWidth;
		int				StretchedHeight;
		bool			UseBitmaps;
		TTFontClass		*MyFontPtr;

		wchar_t		String[ MAX_PATH ];
	    char		NormalBitmap [ _MAX_FNAME ];
		char		PressedBitmap[ _MAX_FNAME ];
		char		FocusBitmap  [ _MAX_FNAME ];
};






#endif				
