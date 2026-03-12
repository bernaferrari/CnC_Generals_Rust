#ifndef _XMOUSE_H
#define _XMOUSE_H

#include	"xmouse.h"

extern Mouse * MouseCursor;



inline void Hide_Mouse(void) {MouseCursor->Hide_Mouse();}
inline void Show_Mouse(void) {MouseCursor->Show_Mouse();}
inline void Conditional_Hide_Mouse(Rect rect) {MouseCursor->Conditional_Hide_Mouse(rect);}
inline void Conditional_Show_Mouse(void) {MouseCursor->Conditional_Show_Mouse();}
inline int Get_Mouse_State(void) {return(MouseCursor->Get_Mouse_State());}
inline void Set_Mouse_Cursor(int hotx, int hoty, ShapeSet const * cursor, int shape) {MouseCursor->Set_Cursor(hotx, hoty, cursor, shape);}
inline int Get_Mouse_X(void) {return(MouseCursor->Get_Mouse_X());}
inline int Get_Mouse_Y(void) {return(MouseCursor->Get_Mouse_Y());}


#endif
