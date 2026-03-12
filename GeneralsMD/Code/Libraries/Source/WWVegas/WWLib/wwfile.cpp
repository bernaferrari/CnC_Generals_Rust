#include <stdio.h>
#include <stdarg.h>
#include <memory.h>
#include "wwfile.h"

#pragma warning(disable : 4514)

int FileClass::Printf(char *str, ...)
{
	char text[PRINTF_BUFFER_SIZE];
	va_list args;
	va_start(args, str);
	int length = _vsnprintf(text, PRINTF_BUFFER_SIZE, str, args);
	va_end(args);
	return Write(text, length);
}

int FileClass::Printf(char *buffer, int bufferSize, char *str, ...)
{
	va_list args;
	va_start(args, str);
	int length = _vsnprintf(buffer, bufferSize, str, args);
	va_end(args);
	return Write(buffer, length);
}

int FileClass::Printf_Indented(unsigned depth, char *str, ...)
{
	char text[PRINTF_BUFFER_SIZE];
	va_list args;
	va_start(args, str);

	if(depth > PRINTF_BUFFER_SIZE) 
		depth = PRINTF_BUFFER_SIZE;

	memset(text, '\t', depth);

	int length;
	if(depth < PRINTF_BUFFER_SIZE) 
		length = _vsnprintf(text + depth, PRINTF_BUFFER_SIZE - depth, str, args);
	else
		length = PRINTF_BUFFER_SIZE;

	va_end(args);

	return Write(text, length + depth);
}

