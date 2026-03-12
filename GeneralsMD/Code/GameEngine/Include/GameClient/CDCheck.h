// FILE: CDCheck.h ////////////////////////////////////////////////////////////////////////////////
// Author: Matt Campbell, January 2003
// Description: check for CD, popping up an in-game message box at game start.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __CDCHECK_H_
#define __CDCHECK_H_

typedef void (*gameStartCallback) (void);

Bool IsFirstCDPresent(void);
void CheckForCDAtGameStart( gameStartCallback callback );

#endif //__CDCHECK_H_