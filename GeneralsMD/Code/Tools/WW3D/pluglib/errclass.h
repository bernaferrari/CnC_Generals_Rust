#ifndef ERRCLASS_H
#define ERRCLASS_H

#include <stdarg.h>


class ErrorClass
{
public:
	ErrorClass(char * format,...);
	ErrorClass(const ErrorClass & that);
	~ErrorClass(void) { if (error_message != NULL) free(error_message); }

	ErrorClass & operator = (const ErrorClass & that);

	char * error_message;
};

inline ErrorClass::ErrorClass(char * format,...)
{
	va_list va;
	char tmp[1024];
	va_start(va,format);
	vsprintf(tmp,format,va);
	assert(strlen(tmp) < 1024);
	va_end(va);
	error_message = strdup(tmp);
}

inline ErrorClass::ErrorClass(const ErrorClass & that)	:
	error_message(NULL)
{
	*this = that;
}

inline ErrorClass & ErrorClass::operator = (const ErrorClass & that)
{
	if (error_message != NULL) {
		free(error_message);
		error_message = NULL;
	}
	
	if (that.error_message != NULL) {
		error_message = strdup(that.error_message);
	}

	return *this;
}


#endif //ERRCLASS_H