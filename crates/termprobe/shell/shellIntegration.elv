use str
use platform

fn __su_osc {|m| print "\e]"$m"\a" }

fn __su_uri_path {|p|
  for b [(str:to-utf8-bytes $p)] {
    var n = (num $b)
    if (or (and (>= $n 48) (<= $n 57)) (and (>= $n 65) (<= $n 90)) (and (>= $n 97) (<= $n 122)) (has-value [45 46 47 58 95 126] (to-string $n))) {
      printf "%c" $n
    } else {
      printf "%%%02X" $n
    }
  }
}
