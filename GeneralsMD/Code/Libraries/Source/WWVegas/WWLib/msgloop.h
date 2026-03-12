#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef MSGLOOP_H
#define MSGLOOP_H

#include <windows.h>

// Main message handler.
void Windows_Message_Handler(void);

// Modeless dialog box support routines.
void Remove_Modeless_Dialog(HWND dialog);
void Add_Modeless_Dialog(HWND dialog);

// Accelerator keys support routines.
void Add_Accelerator(HWND window, HACCEL accelerator);
void Remove_Accelerator(HACCEL accelerator);

// General purpose message intercept handler.
extern bool (*Message_Intercept_Handler)(MSG &msg);

#endif
