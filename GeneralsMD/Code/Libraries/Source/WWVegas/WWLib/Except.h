#pragma once

#ifndef EXCEPT_H
#define EXCEPT_H

#ifdef _MSC_VER

#include "win.h"
/*
** Forward Declarations
*/
typedef struct _EXCEPTION_POINTERS EXCEPTION_POINTERS;
typedef struct _CONTEXT CONTEXT;

int Exception_Handler(int exception_code, EXCEPTION_POINTERS *e_info);
int Stack_Walk(unsigned long *return_addresses, int num_addresses, CONTEXT *context = NULL);
bool Lookup_Symbol(void *code_ptr, char *symbol, int &displacement);
void Load_Image_Helper(void);
void Register_Thread_ID(unsigned long thread_id, char *thread_name, bool main = false);
void Unregister_Thread_ID(unsigned long thread_id, char *thread_name);
void Register_Application_Exception_Callback(void (*app_callback)(void));
void Register_Application_Version_Callback(char *(*app_version_callback)(void));
void Set_Exit_On_Exception(bool set);
bool Is_Trying_To_Exit(void);
unsigned long Get_Main_Thread_ID(void);
#if (0)
bool Register_Thread_Handle(unsigned long thread_id, HANDLE thread_handle);
int Get_Num_Thread(void);
HANDLE Get_Thread_Handle(int thread_index);
#endif //(0)

/*
** Register dump variables. These are used to allow the game to restart from an arbitrary
** position after an exception occurs.
*/
extern unsigned long ExceptionReturnStack;
extern unsigned long ExceptionReturnAddress;
extern unsigned long ExceptionReturnFrame;


typedef struct tThreadInfoType {
	char				ThreadName[128];
	unsigned long	ThreadID;
	HANDLE			ThreadHandle;
	bool				Main;
} ThreadInfoType;



#endif	//_MSC_VER

#endif	//EXCEPT_H