#include "termprobe.h"

int main(void) {
    TuiWaitOptions options = {0};
    options.not_ = true;
    return !options.not_;
}
